import { expect, it } from "vitest";
import type { DesignerRequest, FieldProjection, ScalarEditInput, TrackerCommand } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

it("forwards Auto scalar input and date order unchanged through the worker", async () => {
  let receive: ((event: MessageEvent) => void) | undefined;
  const sent: DesignerRequest[] = [];
  const worker = {
    addEventListener(type: string, callback: (event: MessageEvent) => void) {
      if (type === "message") receive = callback;
    },
    postMessage(message: {id: number; request: DesignerRequest}) {
      sent.push(structuredClone(message.request));
      receive?.({data: {id: message.id, status: "ok", response: {type: "published", payload: {
        base_revision: "resident/0", resulting_revision: "resident/1", entities: [], fields: [], collections: [],
      }}}} as MessageEvent);
    },
  } as unknown as Worker;
  const client = new WorkerDesignerClient(() => worker);
  const target = {entity: "r", field: "mixed"};
  await client.editAuto("resident/0", target, "00123");
  await client.editAuto("resident/1", target, "22/11/2026", "day_first");
  expect(sent).toEqual([
    {type: "edit_scalar", expected_revision: "resident/0", target, input: {kind: "auto", input: "00123"}},
    {type: "edit_scalar", expected_revision: "resident/1", target, input: {kind: "auto", input: "22/11/2026", date_order: "day_first"}},
  ]);
  const paste: TrackerCommand = {type: "paste_cells", expected_revision: "resident/2", collection: "s",
    start_entity: null, start_field: "mixed", rows: [[{kind: "auto", input: "TRUE"}]]};
  await client.trackerCommand(paste);
  expect(sent[2]).toEqual(paste);
  const editable: FieldProjection["editable_scalar"] = "auto";
  expect(editable).toBe("auto");
});

// @ts-expect-error Date order is a closed host-supplied choice.
const invalid: ScalarEditInput = {kind: "auto", input: "22/11/2026", date_order: "locale"};
void invalid;
