import { expect, it } from "vitest";
import type { DesignerRequest, OpenedWindowProjection, ScalarEditInput } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

it("opens a headerless workbook and forwards adjacent positional Auto writes through the worker", async () => {
  let receive: ((event: MessageEvent) => void) | undefined;
  const sent: DesignerRequest[] = [];
  const collection = {id: "s", key: "sheet", display_name: "Sheet 1", headerless: true, data_start_row: 1, entity_count: 0};
  const opened: OpenedWindowProjection = {
    profile: "working_size",
    bootstrap: {title: "Untitled workbook", revision: "resident/0", default_collection: "sheet", collections: [collection], keyed_grouped_sum_definition_ids: []},
    window: {revision: "resident/0", collection, cell_styling: {status: "supported"}, columns: [], columns_revision: "columns/0", total_rows: 0, unfiltered_rows: 0, start: 0, rows: [], column_totals: {}},
  };
  const worker = {
    addEventListener(type: string, callback: (event: MessageEvent) => void) {
      if (type === "message") receive = callback;
    },
    postMessage(message: {id: number; request: DesignerRequest}) {
      sent.push(structuredClone(message.request));
      const response = message.request.type === "new_blank_workbook"
        ? {type: "opened_window", payload: opened}
        : {type: "published", payload: {base_revision: "resident/0", resulting_revision: "resident/1", entities: [], fields: [], schemas: [], schema_fields: [], affected_calculations: [], many: false}};
      receive?.({data: {id: message.id, status: "ok", response}} as MessageEvent);
    },
  } as unknown as Worker;
  const client = new WorkerDesignerClient(() => worker);
  expect(await client.newBlankWorkbook()).toEqual(opened);
  expect(sent[0]).toMatchObject({type: "new_blank_workbook", profile: "working_size"});
  expect(sent[0]).toHaveProperty("occurrence_id", expect.any(String));
  const inputs: [number, number, string][] = [[1, 1, "Item"], [1, 2, "Price"], [2, 1, "Tea"], [2, 2, "120"]];
  for (const [index, [row, column, input]] of inputs.entries()) {
    await client.writeCellAt(`resident/${index}`, "sheet", row, column, {kind: "auto", input});
    expect(sent[index + 1]).toEqual({type: "write_cell_at", expected_revision: `resident/${index}`, collection: "sheet", row, column, input: {kind: "auto", input}});
  }
  const dated: ScalarEditInput = {kind: "auto", input: "22/11/2026", date_order: "day_first"};
  await client.writeCellAt("resident/4", "sheet", 3, 1, dated);
  expect(sent[5]).toMatchObject({row: 3, column: 1, input: dated});
});

// @ts-expect-error A blank workbook always uses working_size.
const wrongProfile: DesignerRequest = {type: "new_blank_workbook", occurrence_id: "o", profile: "ordinary"};
// @ts-expect-error Positional input uses the existing tagged scalar input contract.
const raw: DesignerRequest = {type: "write_cell_at", expected_revision: "r", collection: "s", row: 1, column: 1, input: "text"};
void wrongProfile;
void raw;
