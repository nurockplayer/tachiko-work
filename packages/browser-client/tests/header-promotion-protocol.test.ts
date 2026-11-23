import { expect, it } from "vitest";
import type { DesignerRequest, PublicationProjection } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

it("forwards header promotion and returns publication fallback evidence", async () => {
  let receive: ((event: MessageEvent) => void) | undefined;
  const sent: DesignerRequest[] = [];
  const publication: PublicationProjection = {
    base_revision: "resident/4", resulting_revision: "resident/5",
    schemas: ["s"], schema_fields: [{schema: "s", field: "f"}],
    entities: ["heading"], fields: [], affected_calculations: [],
    header_fallbacks: [{field: "f", name: "Column A", reason: "blank"}],
  };
  const worker = {
    addEventListener(type: string, callback: (event: MessageEvent) => void) {
      if (type === "message") receive = callback;
    },
    postMessage(message: {id: number; request: DesignerRequest}) {
      sent.push(structuredClone(message.request));
      receive?.({data: {id: message.id, status: "ok", response: {type: "published", payload: publication}}} as MessageEvent);
    },
  } as unknown as Worker;
  const client = new WorkerDesignerClient(() => worker);
  expect(await client.promoteFirstRowToHeaders("resident/4", "sheet")).toEqual(publication);
  expect(sent).toEqual([{type: "promote_first_row_to_headers", expected_revision: "resident/4", collection: "sheet"}]);
});

// @ts-expect-error Header promotion is revision-bound.
const missingRevision: DesignerRequest = {type: "promote_first_row_to_headers", collection: "sheet"};
void missingRevision;
