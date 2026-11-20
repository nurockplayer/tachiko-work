import { describe, expect, it, vi } from "vitest";

import type { InteropMetadata } from "../src/runtime/interop-protocol.ts";
import { DesignerRuntimeError } from "../src/runtime/client.ts";
import type { FailureProjection, FieldExplanationProjection, GroupTotalsProjection, TableProjection, WorkerReply, WorkerRequest } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

// This fixture supplies transport events only. Request correlation and promise
// settlement are exercised in the production WorkerDesignerClient, not the fake.
function transport() {
  let onMessage: ((event: MessageEvent<WorkerReply>) => void) | undefined;
  let onError: ((event: ErrorEvent) => void) | undefined;
  const requests: WorkerRequest[] = [];
  const terminate = vi.fn();
  const worker = {
    addEventListener(type: string, listener: unknown) {
      if (type === "message") onMessage = listener as typeof onMessage;
      if (type === "error") onError = listener as typeof onError;
    },
    postMessage(request: WorkerRequest) { requests.push(request); },
    terminate,
  } as unknown as Worker;
  return {
    client: new WorkerDesignerClient(() => worker),
    terminate,
    request(index: number): WorkerRequest {
      const request = requests[index];
      if (request === undefined) throw new Error(`Missing request ${String(index)}`);
      return request;
    },
    reply(reply: WorkerReply) {
      if (onMessage === undefined) throw new Error("Missing message listener");
      onMessage({ data: reply } as MessageEvent<WorkerReply>);
    },
    fail(message: string) {
      if (onError === undefined) throw new Error("Missing error listener");
      onError({ message } as ErrorEvent);
    },
  };
}

function table(collection: string): TableProjection {
  return {
    revision: "resident/0",
    collection: { id: collection, key: collection, display_name: collection, entity_count: 0 },
    columns: [],
    rows: [],
  };
}

function poisonReply(id: number): WorkerReply {
  const unread = (): never => {
    throw new Error("Late reply payload was consumed after settlement.");
  };
  return {
    id,
    get status() {
      return unread();
    },
    get response() {
      return unread();
    },
    get error() {
      return unread();
    },
  };
}

describe("Designer Worker request lifecycle", () => {
  it("dispatches a revision-pinned field explanation and preserves the complete reply", async () => {
    const worker = transport();
    const target = {entity: "row", field: "input"};
    const explanation: FieldExplanationProjection = {
      revision: "resident/3", target,
      value: {target, address: "row.input", stored: {kind: "number", value: 4}, formula: null, calculated: null, diagnostics: [], editable_scalar: "number"},
      formula_source: null, direct_dependencies: [],
      downstream_impacts: [{entity: "row", field: "calculated"}], diagnostics: [],
    };
    const result = worker.client.explainField("resident/3", target);
    expect(worker.request(0)).toMatchObject({kind: "command", request: {type: "explain_field", expected_revision: "resident/3", target}});
    worker.reply({id: worker.request(0).id, status: "ok", response: {type: "field_explanation", payload: explanation}});
    await expect(result).resolves.toEqual(explanation);

    // Schema-only publication is a presentation event even with no rows.
    // This transport fixture checks forwarding; real engine/WASM generation is
    // covered by the native naming and exported-kit regression suites.
    const publication = {base_revision: "resident/3", resulting_revision: "resident/4", schemas: ["orders"], schema_fields: [], entities: [], fields: [], affected_calculations: []};
    const published = worker.client.trackerCommand({type: "undo", expected_revision: "resident/3"});
    worker.reply({id: worker.request(1).id, status: "ok", response: {type: "published", payload: publication}});
    await expect(published).resolves.toEqual(publication);

    const stale = worker.client.explainField("resident/2", target);
    worker.reply({id: worker.request(2).id, status: "error", error: {code: "stale_revision", message: "Stale query", current_revision: "resident/3", diagnostics: []}});
    await expect(stale).rejects.toMatchObject({failure: {code: "stale_revision", current_revision: "resident/3"}});
  });

  it.each([1, 2] as const)("retains the published v%s import mapping for the next export", async version => {
    const worker = transport();
    const sheet = {schema_id: "sheet_1", name: "Renamed", has_header: true};
    const metadata: InteropMetadata = version === 1
      ? {version, sheets: [{...sheet, columns: [{field_id: "value", name: "Value", width: null}], rows: []}]}
      : {version, sheets: [{...sheet, columns: [{field_id: "value", name: "Value"}]}]};
    const publication = {base_revision: "resident/0", resulting_revision: "resident/1", schemas: ["sheet_1"], schema_fields: [], entities: [], fields: [], affected_calculations: [], import_metadata: metadata};
    const renamed = worker.client.renameTable("resident/0", "sheet_1", "Renamed");
    worker.reply({id: worker.request(0).id, status: "ok", response: {type: "published", payload: publication}});
    const result = await renamed;
    expect(result.import_metadata).toEqual(metadata);
    if (result.import_metadata === undefined) throw new Error("Missing published import metadata");
    const exported = worker.client.exportSpreadsheet(result.resulting_revision, result.import_metadata, "xlsx", "sheet_1");
    expect(worker.request(1)).toMatchObject({kind: "spreadsheet", operation: {type: "export", metadata}});
    worker.reply({id: worker.request(1).id, status: "spreadsheet_exported", export: {revision: "resident/1", bytes: new ArrayBuffer(0), ledger: []}});
    await expect(exported).resolves.toMatchObject({revision: "resident/1"});

    const undo = worker.client.trackerCommand({type: "undo", expected_revision: "resident/1"});
    worker.reply({id: worker.request(2).id, status: "ok", response: {type: "published", payload: {...publication, import_metadata: {...metadata, version: 3} as unknown as InteropMetadata}}});
    await expect(undo).rejects.toThrow("Invalid Worker import metadata.");
  });

  it("validates import mappings nested in an add-table publication", async () => {
    const worker = transport();
    const metadata: InteropMetadata = {version: 2, sheets: [{schema_id: "added", name: "Added", has_header: true, columns: [{field_id: "value", name: "Value"}]}]};
    const payload = {collection: {id: "added", key: "added", display_name: "Added", entity_count: 0}, publication: {base_revision: "resident/0", resulting_revision: "resident/1", schemas: ["added"], schema_fields: [], entities: [], fields: [], affected_calculations: [], import_metadata: metadata}};
    const added = worker.client.addTable("resident/0", "Added", [{name: "Value", field_type: "number", required: false}]);
    worker.reply({id: worker.request(0).id, status: "ok", response: {type: "duplicated", payload}});
    await expect(added).resolves.toEqual(payload);
    const invalid = worker.client.addTable("resident/1", "Other", [{name: "Value", field_type: "number", required: false}]);
    worker.reply({id: worker.request(1).id, status: "ok", response: {type: "duplicated", payload: {...payload, publication: {...payload.publication, import_metadata: null as unknown as InteropMetadata}}}});
    await expect(invalid).rejects.toThrow("Invalid Worker import metadata.");
  });

  it("correlates simultaneous table replies by ID rather than arrival order", async () => {
    const worker = transport();
    const first = worker.client.queryTable("first");
    const second = worker.client.queryTable("second");
    const results = Promise.all([first, second]);
    expect(worker.request(0)).toMatchObject({ kind: "command", request: { type: "query_table", collection: "first" } });
    expect(worker.request(1)).toMatchObject({ kind: "command", request: { type: "query_table", collection: "second" } });
    expect(worker.request(0).id).not.toBe(worker.request(1).id);

    worker.reply({ id: worker.request(1).id, status: "ok", response: { type: "table", payload: table("second") } });
    worker.reply({ id: worker.request(0).id, status: "ok", response: { type: "table", payload: table("first") } });
    await expect(results).resolves.toEqual([table("first"), table("second")]);
  });

  it("forwards bounded window and located-in-view requests and validates their replies", async () => {
    const worker = transport();
    const view = {
      sort: [{field: "amount", direction: "desc" as const}],
      filters: [{kind: "number_compare" as const, field: "amount", op: ">=" as const, value: 5.5}],
      keep_entities: ["row-9"],
    };
    const pendingWindow = worker.client.queryTableWindow("resident/7", "items", view, 64, 256);
    expect(worker.request(0)).toMatchObject({
      kind: "command",
      request: {type: "query_table_window", expected_revision: "resident/7", collection: "items", view, start: 64, count: 256},
    });
    const window = {
      ...table("items"),
      columns_revision: "columns/00000000000000ff",
      total_rows: 900,
      unfiltered_rows: 1_000,
      start: 64,
      rows: [],
    };
    worker.reply({id: worker.request(0).id, status: "ok", response: {type: "table_window", payload: window}});
    await expect(pendingWindow).resolves.toEqual(window);

    const pendingLocate = worker.client.locateInView("resident/7", "items", view, "row-9");
    expect(worker.request(1)).toMatchObject({
      kind: "command",
      request: {type: "locate_in_view", expected_revision: "resident/7", collection: "items", view, entity: "row-9"},
    });
    worker.reply({id: worker.request(1).id, status: "ok", response: {type: "located_in_view", payload: {revision: "resident/7", result: "found", index: 12}}});
    await expect(pendingLocate).resolves.toEqual({revision: "resident/7", result: "found", index: 12});

    const malformed = worker.client.locateInView("resident/7", "items", view, "row-1");
    worker.reply({id: worker.request(2).id, status: "ok", response: {type: "located_in_view", payload: {revision: "resident/7", result: "filtered_out", index: 3}}});
    await expect(malformed).rejects.toThrow("Invalid Worker located-in-view result.");
  });

  it("forwards selection-statistics requests and validates their replies", async () => {
    const worker = transport();
    const targets = [{entity: "row-1", field: "amount"}, {entity: "row-9", field: "name"}];
    const pending = worker.client.querySelectionStatistics("resident/7", targets);
    expect(worker.request(0)).toMatchObject({
      kind: "command",
      request: {type: "query_selection_statistics", expected_revision: "resident/7", targets},
    });
    const stats = {revision: "resident/7", count: 2, numerical_count: 1, sum: 4.5, average: 4.5, min: 4.5, max: 4.5, error_count: 0};
    worker.reply({id: worker.request(0).id, status: "ok", response: {type: "selection_statistics", payload: stats}});
    await expect(pending).resolves.toEqual(stats);

    const malformed = worker.client.querySelectionStatistics("resident/7", targets);
    worker.reply({id: worker.request(1).id, status: "ok", response: {type: "selection_statistics", payload: {...stats, average: Number.NaN}}});
    await expect(malformed).rejects.toThrow("Invalid Worker selection statistic.");
  });

  it("forwards column-total requests and validates their replies", async () => {
    const worker = transport();
    const pending = worker.client.queryColumnTotals("resident/7", "items", ["row-1", "row-9"]);
    expect(worker.request(0)).toMatchObject({
      kind: "command",
      request: {type: "query_column_totals", expected_revision: "resident/7", collection: "items", entities: ["row-1", "row-9"]},
    });
    const totals = {revision: "resident/7", rows: 2, column_totals: {amount: 4.5, blank: null}};
    worker.reply({id: worker.request(0).id, status: "ok", response: {type: "column_totals", payload: totals}});
    await expect(pending).resolves.toEqual(totals);

    const malformed = worker.client.queryColumnTotals("resident/7", "items", ["row-1"]);
    worker.reply({id: worker.request(1).id, status: "ok", response: {type: "column_totals", payload: {revision: "resident/7", rows: 1, column_totals: {amount: Number.NaN}}}});
    await expect(malformed).rejects.toThrow("Invalid Worker column total.");
  });

  it("forwards group-total requests and validates their replies", async () => {
    const worker = transport();
    const dimensions = [{field: "category"}, {field: "when", bucket: "month" as const}];
    const scope = {kind: "except" as const, entities: ["row-1"]};
    const pending = worker.client.queryGroupTotals("resident/7", "items", dimensions, ["amount"], scope);
    expect(worker.request(0)).toMatchObject({
      kind: "command",
      request: {type: "query_group_totals", expected_revision: "resident/7", collection: "items", dimensions, measures: ["amount"], scope},
    });
    const totals: GroupTotalsProjection = {
      revision: "resident/7",
      dimensions: [{field: "category", bucket: "value"}, {field: "when", bucket: "month"}],
      measures: ["amount"],
      rows: 2,
      totals: [4.5],
      groups: [
        {keys: [{kind: "text", value: "Food"}, {kind: "month", value: "2026-03"}], rows: 1, totals: [4.5]},
        {keys: [{kind: "blank"}, {kind: "blank"}], rows: 1, totals: [0]},
      ],
    };
    worker.reply({id: worker.request(0).id, status: "ok", response: {type: "group_totals", payload: totals}});
    await expect(pending).resolves.toEqual(totals);

    const malformed = worker.client.queryGroupTotals("resident/7", "items", dimensions, ["amount"], scope);
    worker.reply({id: worker.request(1).id, status: "ok", response: {type: "group_totals", payload: {...totals, groups: [{keys: [{kind: "text", value: "Food"}, {kind: "month", value: "2026-03"}], rows: 1, totals: [Number.NaN]}]}}});
    await expect(malformed).rejects.toThrow("Invalid Worker group total.");
  });

  it("ignores unknown and duplicate replies without consuming another pending request", async () => {
    const worker = transport();
    const first = worker.client.queryTable("first");
    const second = worker.client.queryTable("second");
    const results = Promise.all([first, second]);
    worker.reply({ id: -1, status: "ok", response: { type: "table", payload: table("unknown") } });
    worker.reply({ id: worker.request(0).id, status: "ok", response: { type: "table", payload: table("first") } });
    worker.reply({ id: worker.request(0).id, status: "ok", response: { type: "table", payload: table("duplicate") } });
    worker.reply({ id: worker.request(1).id, status: "ok", response: { type: "table", payload: table("second") } });
    await expect(results).resolves.toEqual([table("first"), table("second")]);
  });

  it.each([
    ["worker crashed", "worker crashed"],
    ["", "The Designer Worker stopped unexpectedly."],
  ])("rejects every in-flight request on Worker error %j", async (message, expected) => {
    const worker = transport();
    const results = Promise.allSettled([
      worker.client.queryTable("first"),
      worker.client.queryTable("second"),
    ]);
    worker.fail(message);
    const settled = await results;
    expect(settled).toHaveLength(2);
    for (const result of settled) {
      expect(result.status).toBe("rejected");
      if (result.status !== "rejected") throw new Error("Worker failure resolved a request");
      expect(result.reason).toBeInstanceOf(Error);
      expect(result.reason).toHaveProperty("message", expected);
    }
  });

  it("ignores a late reply after Worker error rejects a request without reading its payload", async () => {
    const worker = transport();
    const result = worker.client.queryTable("worker-error");
    const id = worker.request(0).id;
    worker.fail("worker crashed");
    await expect(result).rejects.toThrow("worker crashed");
    expect(() => {
      worker.reply(poisonReply(id));
    }).not.toThrow();
  });

  it("terminates the Worker and rejects all pending requests when the client closes", async () => {
    const worker = transport();
    const results = Promise.allSettled([
      worker.client.queryTable("first"),
      worker.client.exportProject("resident/0"),
      worker.client.exportProjectV3("resident/0"),
    ]);
    worker.client.close();
    expect(worker.terminate).toHaveBeenCalledOnce();
    const settled = await results;
    expect(settled).toHaveLength(3);
    for (const result of settled) {
      expect(result.status).toBe("rejected");
      if (result.status !== "rejected") throw new Error("Closing the client resolved a request");
      expect(result.reason).toBeInstanceOf(Error);
      expect(result.reason).toHaveProperty("message", "The Designer runtime was closed.");
    }
  });

  it("retains structured runtime failure details and rejects only the addressed request", async () => {
    const worker = transport();
    const failure: FailureProjection = {
      code: "stale_revision",
      message: "The requested revision is stale.",
      current_revision: "resident/1",
      diagnostics: [{ code: "test.stale", message: "Refresh before editing.", path: "items.value" }],
    };
    const rejected = worker.client.queryFields("resident/0", []);
    const successful = worker.client.queryTable("current");
    const results = Promise.allSettled([rejected, successful]);
    worker.reply({ id: worker.request(0).id, status: "error", error: failure });
    worker.reply({ id: worker.request(1).id, status: "ok", response: { type: "table", payload: table("current") } });
    const [first, second] = await results;
    expect(first.status).toBe("rejected");
    if (first.status !== "rejected") throw new Error("Runtime failure resolved a request");
    expect(first.reason).toBeInstanceOf(DesignerRuntimeError);
    expect(first.reason).toMatchObject({ message: failure.message, failure });
    expect(second).toEqual({ status: "fulfilled", value: table("current") });
  });

  it("rejects an unexpected response type instead of exposing the wrong projection", async () => {
    const worker = transport();
    const result = expect(worker.client.queryTable("items")).rejects.toThrow(
      "Expected 'table' response, received 'fields'.",
    );
    worker.reply({ id: worker.request(0).id, status: "ok", response: { type: "fields", payload: { revision: "resident/0", fields: [] } } });
    await result;
  });

  it("ignores a duplicate reply after completion without reading its payload", async () => {
    const worker = transport();
    const result = worker.client.queryTable("completed");
    const id = worker.request(0).id;
    worker.reply({ id, status: "ok", response: { type: "table", payload: table("completed") } });
    await expect(result).resolves.toEqual(table("completed"));
    expect(() => {
      worker.reply(poisonReply(id));
    }).not.toThrow();
  });

  it("ignores a late reply after an error rejection without reading its payload", async () => {
    const worker = transport();
    const result = worker.client.queryTable("error");
    const id = worker.request(0).id;
    const failure: FailureProjection = {
      code: "test.failure",
      message: "The request failed for this lifecycle test.",
      current_revision: "resident/0",
      diagnostics: [],
    };
    worker.reply({ id, status: "error", error: failure });
    await expect(result).rejects.toMatchObject({ message: failure.message, failure });
    expect(() => {
      worker.reply(poisonReply(id));
    }).not.toThrow();
  });

  it("ignores a late reply after close rejects a request without reading its payload", async () => {
    const worker = transport();
    const result = worker.client.queryTable("closed");
    const id = worker.request(0).id;
    worker.client.close();
    await expect(result).rejects.toThrow("The Designer runtime was closed.");
    expect(() => {
      worker.reply(poisonReply(id));
    }).not.toThrow();
  });
});
