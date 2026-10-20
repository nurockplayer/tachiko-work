import { afterEach, describe, expect, it, vi } from "vitest";
import { createExperimentalDesignerClient } from "../src/experimental-client.ts";
import type { FieldProjection, TableProjection, WorkerRequest } from "../src/runtime/protocol.ts";

afterEach(() => vi.unstubAllGlobals());

const target = {entity: "row", field: "note"};
const present: FieldProjection = {
  target, address: "items.row.note", stored: {kind: "text", value: ""},
  formula: null, calculated: null, diagnostics: [], editable_scalar: "text",
};
const blank: FieldProjection = {...present, blank: true, stored: null};
const table: TableProjection = {
  revision: "resident/0", collection: {id: "items", key: "items", display_name: "items", entity_count: 1},
  columns: [{id: "note", key: "note", display_name: "note", field_type: "text", required: false}],
  rows: [{id: "row", key: "row", fields: [], blank_fields: ["note"]}],
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
  const publication = {base_revision: "resident/0", resulting_revision: "resident/1", schemas: [], schema_fields: [], entities: [], fields: [target], affected_calculations: []};
  async function published(pending: Promise<unknown>) { reply("published", publication); await expect(pending).resolves.toEqual(publication); }
  return {client, requests, reply, published};
}

describe("public optional-field requests", () => {
  it("encodes clear separately from empty Text, including mixed batch edits and typed/raw paste", async () => {
    const {client, requests, published} = fixture();
    await published(client.clearField("resident/0", target));
    await published(client.editText("resident/0", target, ""));
    const edits = [{target, input: {kind: "clear" as const}}, {target: {...target, entity: "other"}, input: {kind: "text" as const, value: ""}}];
    await published(client.trackerCommand!({type: "edit_cells", expected_revision: "resident/0", edits}));
    const rows = [["", {kind: "clear" as const}, {kind: "text" as const, value: ""}, {kind: "number" as const, input: "0"}]];
    await published(client.trackerCommand!({type: "paste_cells", expected_revision: "resident/0", collection: "items", start_entity: "row", start_field: "note", rows}));
    expect(requests).toEqual([
      {id: 1, kind: "command", request: {type: "edit_scalar", expected_revision: "resident/0", target, input: {kind: "clear"}}},
      {id: 2, kind: "command", request: {type: "edit_scalar", expected_revision: "resident/0", target, input: {kind: "text", value: ""}}},
      {id: 3, kind: "command", request: {type: "edit_cells", expected_revision: "resident/0", edits}},
      {id: 4, kind: "command", request: {type: "paste_cells", expected_revision: "resident/0", collection: "items", start_entity: "row", start_field: "note", rows}},
    ]);
    client.close();
  });

  it("preserves omitted defaults, optional declarations and initializer omission, and explicit requiredness", async () => {
    const {client, requests, reply, published} = fixture();
    const columns = [{name: "label", field_type: "text"}, {name: "note", field_type: "text", required: false}];
    const pending = client.newTable!("items", columns);
    reply("opened", {bootstrap: {}, table});
    await pending;
    expect(requests[0]).toMatchObject({kind: "command", request: {type: "new_table", name: "items", columns}});
    expect(JSON.stringify(requests[0])).not.toContain('"required":true');
    await published(client.trackerCommand!({type: "add_column", expected_revision: "resident/0", collection: "items", name: "amount", field_type: "number", required: false}));
    expect(requests[1]).toEqual({id: 2, kind: "command", request: {type: "add_column", expected_revision: "resident/0", collection: "items", name: "amount", field_type: "number", required: false}});
    for (const required of [true, false]) {
      await published(client.setColumnRequiredness("resident/0", "items", "note", required));
      expect(requests.at(-1)).toMatchObject({kind: "command", request: {type: "set_column_requiredness", expected_revision: "resident/0", collection: "items", field: "note", required}});
    }
    client.close();
  });
});

describe("Worker presence projections", () => {
  it("preserves blank metadata, present empty Text and omitted false/empty defaults without inserting placeholders", async () => {
    const {client, reply} = fixture();
    const pending = client.queryTable("items");
    reply("table", table);
    await expect(pending).resolves.toEqual(table);
    const fields = client.queryFields("resident/0", [target, {...target, entity: "other"}]);
    reply("fields", {revision: "resident/0", fields: [blank, {...present, target: {...target, entity: "other"}}]});
    const result = await fields;
    expect(result.fields[0]).toEqual(blank);
    expect(result.fields[1]?.blank ?? false).toBe(false);
    expect(result.fields[1]?.stored).toEqual({kind: "text", value: ""});
    const filled = client.queryTable("items");
    const filledTable = {...table, rows: [{id: "row", key: "row", fields: [{...present, blank: false}]}]};
    reply("table", filledTable);
    await expect(filled).resolves.toEqual(filledTable);
    client.close();
  });

  it.each([undefined, null, "false", 0])("rejects malformed column requiredness %s", async required => {
    const {client, reply} = fixture();
    const pending = client.queryTable("items");
    reply("table", {...table, columns: [{...table.columns[0], required}]});
    await expect(pending).rejects.toThrow("column requiredness");
    client.close();
  });

  it.each([null, "note", [1], ["unknown"], ["note", "note"]])("rejects malformed row blanks %j", async blank_fields => {
    const {client, reply} = fixture();
    const pending = client.queryTable("items");
    reply("table", {...table, rows: [{...table.rows[0], blank_fields}]});
    await expect(pending).rejects.toThrow("Invalid Worker");
    client.close();
  });

  it.each([null, "true", 1])("rejects malformed field blank flag %s", async blank => {
    const {client, reply} = fixture();
    const pending = client.queryFields("resident/0", [target]);
    reply("fields", {revision: "resident/0", fields: [{...present, blank}]});
    await expect(pending).rejects.toThrow("field blank flag");
    client.close();
  });

  it("rejects blank/present contradictions and blank required columns", async () => {
    const {client, reply} = fixture();
    const fields = client.queryFields("resident/0", [target]);
    reply("fields", {revision: "resident/0", fields: [{...present, blank: true}]});
    await expect(fields).rejects.toThrow("blank field payload");
    for (const invalid of [
      {...table, columns: [{...table.columns[0], required: true}]},
      {...table, rows: [{...table.rows[0], fields: [present]}]},
    ]) {
      const pending = client.queryTable("items");
      reply("table", invalid);
      await expect(pending).rejects.toThrow("sparse row presence");
    }
    client.close();
  });

  it("also validates opened tables and field explanations, including dependencies", async () => {
    const {client, reply} = fixture();
    const opened = client.newTable!("items", [{name: "note", field_type: "text", required: false}]);
    reply("opened", {bootstrap: {}, table: {...table, columns: [{...table.columns[0], required: "false"}]}});
    await expect(opened).rejects.toThrow("column requiredness");
    const explanation = {revision: "resident/0", target, value: blank, formula_source: null, direct_dependencies: [], downstream_impacts: [], diagnostics: []};
    const accepted = client.explainField("resident/0", target);
    reply("field_explanation", explanation);
    await expect(accepted).resolves.toEqual(explanation);
    const rejected = client.explainField("resident/0", target);
    reply("field_explanation", {...explanation, direct_dependencies: [{target, value: {...present, blank: "true"}}]});
    await expect(rejected).rejects.toThrow("field blank flag");
    client.close();
  });
});
