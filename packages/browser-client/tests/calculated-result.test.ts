import { describe, expect, it } from "vitest";
import type { FieldProjection, WorkerReply, WorkerRequest } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

const target = {entity: "row", field: "formula"};

function transport() {
  let receive: ((event: MessageEvent<WorkerReply>) => void) | undefined;
  let requestId = 0;
  const worker = {
    addEventListener(type: string, callback: unknown) {
      if (type === "message") receive = callback as typeof receive;
    },
    postMessage(request: WorkerRequest) { requestId = request.id; },
    terminate() {},
  } as unknown as Worker;
  return {
    client: new WorkerDesignerClient(() => worker),
    fields(calculated: unknown) {
      // Malformed calculated results are sent on purpose, so the reply is cast.
      const data = {
        id: requestId, status: "ok", response: {type: "fields", payload: {
          revision: "resident/0", fields: [{
            target, address: "items.row.formula", stored: null, formula: {source: "2.5"},
            calculated, diagnostics: [], editable_scalar: null,
          }],
        }},
      } as unknown as WorkerReply;
      receive!(new MessageEvent("message", {data}));
    },
  };
}

describe("calculated result validation", () => {
  it("accepts absent result as Number and valid stored-value tags, preserves failures, and rejects malformed tags", async () => {
    const fixture = transport();
    const valid: Array<FieldProjection["calculated"]> = [
      {status: "value", value: 2.5},
      {status: "value", value: 2.5, result: {kind: "number", value: 2.5}},
      {status: "value", value: 2.5, result: {kind: "text", value: "hello"}},
      {status: "value", value: 2.5, result: {kind: "boolean", value: false}},
      {status: "value", value: 2.5, result: {kind: "date", value: "2026-01-02"}},
      {status: "value", value: 2.5, result: {kind: "date", value: "2024-02-29"}},
      {status: "value", value: 2.5, result: {kind: "reference", entity: "other"}},
      {status: "failure", code: "division_by_zero", message: "division by zero"},
      {status: "unavailable"},
    ];
    for (const calculated of valid) {
      const pending = fixture.client.queryFields("resident/0", [target]);
      fixture.fields(calculated);
      const projection = await pending;
      expect(projection.fields[0]?.calculated).toEqual(calculated);
      if (calculated?.status === "value") {
        expect(projection.fields[0]?.calculated).toHaveProperty("value", 2.5);
      }
      if (calculated?.status === "value" && calculated.result === undefined) {
        expect(projection.fields[0]?.calculated).not.toHaveProperty("result");
      }
    }
    for (const result of [
      null, [], "date", {}, {kind: "unknown", value: 0},
      {kind: "number", value: "2.5"}, {kind: "number", value: Number.NaN},
      {kind: "number", value: Number.POSITIVE_INFINITY}, {kind: "number"},
      {kind: "text", value: false}, {kind: "boolean", value: 1},
      {kind: "reference", entity: 1}, {kind: "date", value: 20260102},
      {kind: "date", value: "2026-1-2"}, {kind: "date", value: "2026-02-30"},
      {kind: "date", value: "2025-02-29"}, {kind: "date", value: "0000-01-01"},
    ]) {
      const pending = fixture.client.queryFields("resident/0", [target]);
      fixture.fields({status: "value", value: 2.5, result});
      await expect(pending).rejects.toThrow("Invalid Worker calculated result.");
    }
    fixture.client.close();
  });
});
