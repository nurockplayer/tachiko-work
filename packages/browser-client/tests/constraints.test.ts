import { afterEach, describe, expect, it, vi } from "vitest";
import { DesignerRuntimeError, createExperimentalDesignerClient } from "../src/experimental-client.ts";
import type { ColumnProjection, FailureProjection, TableProjection, WorkerRequest } from "../src/runtime/protocol.ts";

afterEach(() => vi.unstubAllGlobals());

const status: ColumnProjection = {
  id: "field_status", key: "status", display_name: "Status", field_type: "text", required: true,
  dropdown_options: ["Doing", "Done", "Todo"],
  constraint: {kind: "text_literal_set", values: ["Doing", "Done", "Todo"]},
};
const amount: ColumnProjection = {
  id: "field_amount", key: "amount", display_name: "Amount", field_type: "number", required: true,
  constraint: {kind: "number_inclusive_range", min: 0, max: 100},
};
const table: TableProjection = {
  revision: "resident/0", collection: {id: "tasks", key: "tasks", display_name: "Tasks", entity_count: 0},
  columns: [status, amount],
  rows: [],
};

function fixture() {
  let receive: ((event: MessageEvent) => void) | undefined;
  const requests: WorkerRequest[] = [];
  vi.stubGlobal("Worker", class {
    addEventListener(type: string, callback: (event: MessageEvent) => void) {
      if (type === "message") receive = callback;
    }
    postMessage(request: WorkerRequest) { requests.push(structuredClone(request)); }
    terminate() {}
  });
  const client = createExperimentalDesignerClient();
  function reply(type: string, payload: unknown) {
    receive!(new MessageEvent("message", {data: {id: requests.at(-1)!.id, status: "ok", response: {type, payload}}}));
  }
  function fail(error: FailureProjection) {
    receive!(new MessageEvent("message", {data: {id: requests.at(-1)!.id, status: "error", error}}));
  }
  const publication = {base_revision: "resident/0", resulting_revision: "resident/1", schemas: [], schema_fields: [], entities: [], fields: [], affected_calculations: []};
  async function published(pending: Promise<unknown>) { reply("published", publication); await expect(pending).resolves.toEqual(publication); }
  return {client, requests, reply, fail, published};
}

describe("public field-constraint requests", () => {
  it("encodes list, range and clear declarations as complete replacements", async () => {
    const {client, requests, published} = fixture();
    const list = {kind: "text_literal_set" as const, values: ["Doing", "Done", "Todo"]};
    const range = {kind: "number_inclusive_range" as const, min: 0, max: 100};
    await published(client.setColumnConstraint("resident/0", "tasks", "status", list));
    await published(client.setColumnConstraint("resident/1", "tasks", "amount", range));
    await published(client.setColumnConstraint("resident/2", "tasks", "status", {kind: "none"}));
    expect(requests).toEqual([
      {id: 1, kind: "command", request: {type: "set_column_constraint", expected_revision: "resident/0", collection: "tasks", field: "status", constraint: list}},
      {id: 2, kind: "command", request: {type: "set_column_constraint", expected_revision: "resident/1", collection: "tasks", field: "amount", constraint: range}},
      {id: 3, kind: "command", request: {type: "set_column_constraint", expected_revision: "resident/2", collection: "tasks", field: "status", constraint: {kind: "none"}}},
    ]);
    client.close();
  });

  it("preserves declared constraints and literal dropdowns exactly", async () => {
    const {client, reply} = fixture();
    const pending = client.queryTable("tasks");
    reply("table", table);
    await expect(pending).resolves.toEqual(table);
    // Unconstrained and boolean columns omit the constraint entirely.
    const unconstrained = {...table, columns: [
      {...status, constraint: undefined, dropdown_options: undefined},
      {id: "flag", key: "flag", display_name: "flag", field_type: "boolean", required: true, dropdown_options: ["true", "false"]},
    ]};
    const plain = client.queryTable("tasks");
    reply("table", unconstrained);
    await expect(plain).resolves.toEqual(unconstrained);
    client.close();
  });

  it.each([
    {kind: "unknown"},
    {kind: "text_literal_set"},
    {kind: "text_literal_set", values: [1]},
    {kind: "number_inclusive_range", min: 100, max: 0},
    {kind: "number_inclusive_range", min: Number.NaN, max: 100},
    {kind: "number_inclusive_range", min: 0, max: Number.POSITIVE_INFINITY},
  ])("rejects malformed constraint projections %j", async constraint => {
    const {client, reply} = fixture();
    const pending = client.queryTable("tasks");
    reply("table", {...table, columns: [{...status, constraint} as unknown as ColumnProjection]});
    await expect(pending).rejects.toThrow("column constraint");
    client.close();
  });

  const semanticMismatches: Array<[string, ColumnProjection]> = [
    ["empty literal list", {...status, constraint: {kind: "text_literal_set", values: []}, dropdown_options: []}],
    ["unsorted literal list", {...status, constraint: {kind: "text_literal_set", values: ["Todo", "Done"]}, dropdown_options: ["Todo", "Done"]}],
    ["duplicate literals", {...status, constraint: {kind: "text_literal_set", values: ["Done", "Done"]}, dropdown_options: ["Done", "Done"]}],
    ["literal list on a number column", {
      id: amount.id, key: amount.key, display_name: amount.display_name, field_type: "number", required: true,
      dropdown_options: ["Doing", "Done"], constraint: {kind: "text_literal_set", values: ["Doing", "Done"]},
    }],
    ["range on a text column", {
      id: status.id, key: status.key, display_name: status.display_name, field_type: "text", required: true,
      constraint: {kind: "number_inclusive_range", min: 0, max: 100},
    }],
    ["dropdown that disagrees with the literals", {
      ...status, constraint: {kind: "text_literal_set", values: ["Doing", "Done"]},
      dropdown_options: ["Doing", "Done", "Todo"],
    }],
  ];
  it.each(semanticMismatches)("rejects semantically invalid projections: %s", async (_label, column) => {
    const {client, reply} = fixture();
    const pending = client.queryTable("tasks");
    reply("table", {...table, columns: [column]});
    await expect(pending).rejects.toThrow("column constraint");
    client.close();
  });

  it("retains the typed refusal shape for off-constraint values", async () => {
    const {client, fail} = fixture();
    const failure: FailureProjection = {
      code: "field_constraint_violation",
      message: "field 'tasks.status' refuses this value outside its constraint (allowed values: Doing, Done, Todo)",
      current_revision: "resident/1",
      diagnostics: [],
      constraint_failure: {
        reason: "field_value", schema: "tasks", field: "status",
        constraint: {kind: "text_literal_set", values: ["Doing", "Done", "Todo"]}, offending_rows: 1,
      },
    };
    const rejected = client.editText("resident/1", {entity: "row", field: "status"}, "Blocked");
    fail(failure);
    await expect(rejected).rejects.toBeInstanceOf(DesignerRuntimeError);
    await expect(rejected).rejects.toMatchObject({failure});
    client.close();
  });

  it("retains a declaration refusal with the offending row count", async () => {
    const {client, fail, requests} = fixture();
    const failure: FailureProjection = {
      code: "existing_values_violate_constraint",
      message: "refused to set the constraint on 'tasks.status': 2 existing row(s) are outside it (allowed values: Doing, Done, Todo)",
      current_revision: "resident/4",
      diagnostics: [],
      constraint_failure: {
        reason: "existing_values", schema: "tasks", field: "status",
        constraint: {kind: "text_literal_set", values: ["Doing", "Done", "Todo"]}, offending_rows: 2,
      },
    };
    const rejected = client.setColumnConstraint("resident/4", "tasks", "status", {kind: "text_literal_set", values: ["Doing", "Done", "Todo"]});
    fail(failure);
    await expect(rejected).rejects.toMatchObject({failure});
    expect(requests.at(-1)).toMatchObject({kind: "command", request: {type: "set_column_constraint"}});
    client.close();
  });
});
