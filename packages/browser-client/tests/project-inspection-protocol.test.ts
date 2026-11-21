import { expect, expectTypeOf, it } from "vitest";
import type { InteropMetadata } from "../src/runtime/interop-protocol.ts";
import type { DesignerRequest, ProjectInspectionProjection } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

const metadata: InteropMetadata = {version: 2, sheets: [
  {schema_id: "input", name: "Renamed Input", has_header: true, columns: [{field_id: "value", name: "Value"}]},
  {schema_id: "scratch", name: "Scratch", has_header: true, columns: [{field_id: "value", name: "Value"}]},
]};

it.each([false, true])("types and transports optional resident import metadata (%s)", async present => {
  expectTypeOf<ProjectInspectionProjection["import_metadata"]>().toEqualTypeOf<InteropMetadata | undefined>();
  const payload: ProjectInspectionProjection = {revision: "resident/1", ...(present ? {import_metadata: metadata} : {})};
  const sent: DesignerRequest[] = [];
  let receive: ((event: MessageEvent) => void) | undefined;
  const worker = {
    addEventListener(type: string, callback: (event: MessageEvent) => void) {
      if (type === "message") receive = callback;
    },
    postMessage(message: {id: number; request: DesignerRequest}) {
      sent.push(message.request);
      receive?.({data: {id: message.id, status: "ok", response: {type: "project_inspected", payload}}} as MessageEvent);
    },
  } as unknown as Worker;
  const client = new WorkerDesignerClient(() => worker);
  const inspected = await client.inspectCurrentProject();
  expect(sent).toEqual([{type: "inspect_project"}]);
  expect(inspected).toEqual(payload);
  expect(Object.hasOwn(inspected, "import_metadata")).toBe(present);
});
