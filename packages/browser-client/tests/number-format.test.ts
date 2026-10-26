import { afterEach, describe, expect, it, vi } from "vitest";
import { createExperimentalDesignerClient } from "../src/experimental-client.ts";
import type { ColumnProjection, NumberFormat, TableProjection, WorkerRequest } from "../src/runtime/protocol.ts";

afterEach(() => vi.unstubAllGlobals());

const amount: ColumnProjection = {
  id: "field_amount", key: "amount", display_name: "Amount", field_type: "number", required: true,
};
const table: TableProjection = {
  revision: "resident/0", collection: {id: "tasks", key: "tasks", display_name: "Tasks", entity_count: 0},
  columns: [amount], rows: [],
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
  const publication = {base_revision: "resident/0", resulting_revision: "resident/1", schemas: [], schema_fields: [], entities: [], fields: [], affected_calculations: []};
  async function published(pending: Promise<unknown>) {
    reply("published", publication);
    await expect(pending).resolves.toEqual(publication);
  }
  return {client, requests, reply, published};
}

describe("public number-format requests", () => {
  it("encodes complete formats and null clears", async () => {
    const {client, requests, published} = fixture();
    const format: NumberFormat = {decimals: 2, kind: {type: "currency", code: "JPY"}, thousands_separator: true};
    await published(client.setColumnNumberFormat("resident/0", "tasks", "amount", format));
    await published(client.setColumnNumberFormat("resident/1", "tasks", "amount", null));
    expect(requests).toEqual([
      {id: 1, kind: "command", request: {type: "set_column_number_format", expected_revision: "resident/0", collection: "tasks", field: "amount", number_format: format}},
      {id: 2, kind: "command", request: {type: "set_column_number_format", expected_revision: "resident/1", collection: "tasks", field: "amount", number_format: null}},
    ]);
    client.close();
  });

  it("preserves valid projected formats and accepts omission", async () => {
    const {client, reply} = fixture();
    const format: NumberFormat = {decimals: 0, kind: {type: "percent"}, thousands_separator: false};
    const pending = client.queryTable("tasks");
    reply("table", {...table, columns: [{...amount, number_format: format}]});
    await expect(pending).resolves.toMatchObject({columns: [{number_format: format}]});
    const absent = client.queryTable("tasks");
    reply("table", table);
    await expect(absent).resolves.toEqual(table);
    client.close();
  });

  it.each([
    {decimals: -1, kind: {type: "plain"}, thousands_separator: true},
    {decimals: 11, kind: {type: "plain"}, thousands_separator: true},
    {decimals: 1.5, kind: {type: "plain"}, thousands_separator: true},
    {decimals: 2, kind: {type: "currency", code: "usd"}, thousands_separator: true},
    {decimals: 2, kind: {type: "currency", code: "US"}, thousands_separator: true},
    {decimals: 2, kind: {type: "currency", code: "USD\n"}, thousands_separator: true},
    {decimals: 2, kind: {type: "unknown"}, thousands_separator: true},
    {decimals: 2, kind: {type: "plain"}, thousands_separator: 1},
  ])("rejects malformed number-format projections %j", async format => {
    const {client, reply} = fixture();
    const pending = client.queryTable("tasks");
    reply("table", {...table, columns: [{...amount, number_format: format} as unknown as ColumnProjection]});
    await expect(pending).rejects.toThrow("column number format");
    client.close();
  });

  it("rejects a number format on non-number columns", async () => {
    const {client, reply} = fixture();
    const pending = client.queryTable("tasks");
    reply("table", {...table, columns: [{...amount, field_type: "text", number_format: {decimals: 2, kind: {type: "plain"}, thousands_separator: true}}]});
    await expect(pending).rejects.toThrow("column number format");
    client.close();
  });
});
