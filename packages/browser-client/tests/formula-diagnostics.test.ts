import { describe, expect, it } from "vitest";

import { DesignerRuntimeError } from "../src/runtime/client.ts";
import { isFormulaFailure, type FailureProjection, type WorkerReply, type WorkerRequest } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

// This fixture supplies transport events only; the refusal content under test
// is the Rust projection as it crosses the Worker boundary.
function transport() {
  let onMessage: ((event: MessageEvent<WorkerReply>) => void) | undefined;
  const requests: WorkerRequest[] = [];
  const worker = {
    addEventListener(type: string, listener: unknown) {
      if (type === "message") onMessage = listener as typeof onMessage;
    },
    postMessage(request: WorkerRequest) { requests.push(request); },
    terminate() { /* no transport state to release */ },
  } as unknown as Worker;
  return {
    client: new WorkerDesignerClient(() => worker),
    request(index: number): WorkerRequest {
      const request = requests[index];
      if (request === undefined) throw new Error(`Missing request ${String(index)}`);
      return request;
    },
    reply(reply: WorkerReply) {
      if (onMessage === undefined) throw new Error("Missing message listener");
      onMessage({ data: reply } as MessageEvent<WorkerReply>);
    },
  };
}

const codes = ["formula_syntax", "formula_unknown_reference", "formula_type_mismatch", "formula_cycle"] as const;

describe("formula refusal codes", () => {
  it.each(codes)("threads %s with its human message and diagnostic through the client", async (code) => {
    const failure: FailureProjection = {
      code,
      message: "The formula was refused.",
      current_revision: "resident/1",
      diagnostics: [{ code, message: "The formula was refused.", path: "formulas.Plan.Rent" }],
    };
    const transportFixture = transport();
    const refused = transportFixture.client.updateFormula("resident/1", { entity: "rent", field: "reserve" }, "[rent.expense] *");
    expect(transportFixture.request(0)).toMatchObject({ kind: "command", request: { type: "formula_update" } });
    transportFixture.reply({ id: transportFixture.request(0).id, status: "error", error: failure });
    const error = await refused.catch((reason: unknown) => reason);
    expect(error).toBeInstanceOf(DesignerRuntimeError);
    const rejected = error as DesignerRuntimeError;
    expect(rejected.failure).toEqual(failure);
    expect(rejected.message).toBe(failure.message);
    expect(isFormulaFailure(rejected.failure)).toBe(true);
  });

  it("does not classify other refusals as formula failures", () => {
    expect(isFormulaFailure({ code: "stale_revision", message: "Stale.", current_revision: "resident/1", diagnostics: [] })).toBe(false);
  });
});
