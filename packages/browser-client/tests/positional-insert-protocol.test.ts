import { expect, it } from "vitest";
import type { DesignerRequest, TrackerCommand } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

it("passes insert positions and count unchanged through the worker", async () => {
  let receive: ((event: MessageEvent) => void) | undefined;
  const sent: DesignerRequest[] = [];
  const worker = {
    addEventListener(type: string, callback: (event: MessageEvent) => void) {
      if (type === "message") receive = callback;
    },
    postMessage(message: {id: number; kind: string; request: DesignerRequest}) {
      sent.push(structuredClone(message.request));
      receive?.({data: {id: message.id, status: "ok", response: {type: "published", payload: {
        base_revision: "resident/0", resulting_revision: "resident/1", entities: [], fields: [], collections: [],
      }}}} as MessageEvent);
    },
  } as unknown as Worker;
  const client = new WorkerDesignerClient(() => worker);
  const base = {type: "insert_row", expected_revision: "resident/0", collection: "rows", initializers: []} as const;
  const requests: TrackerCommand[] = [
    {...base, initializers: [], position: {before: "anchor"}},
    {...base, initializers: [], position: {after: "anchor"}},
    {...base, initializers: []},
    {...base, initializers: [], position: {before: "anchor"}, count: 3},
  ];
  for (const request of requests) await client.trackerCommand(request);
  expect(sent).toEqual(requests);
  expect(sent[2]).not.toHaveProperty("position");
});

// @ts-expect-error A position must name exactly one direction.
const both: TrackerCommand = {type: "insert_row", expected_revision: "r", collection: "rows", initializers: [], position: {before: "a", after: "b"}};
// @ts-expect-error An empty position has no anchor.
const neither: TrackerCommand = {type: "insert_row", expected_revision: "r", collection: "rows", initializers: [], position: {}};
void both;
void neither;
