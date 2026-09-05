import { describe, expect, it, vi } from "vitest";

import { DesignerRuntimeError } from "../src/runtime/client.ts";
import type { FailureProjection, TableProjection, WorkerReply, WorkerRequest } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

// This fixture supplies transport events only. Request correlation and promise
// settlement are exercised in the production WorkerDesignerClient, not the fake.
function transport() {
  let onMessage: ((event: MessageEvent<WorkerReply>) => void) | undefined;
  let onError: ((event: ErrorEvent) => void) | undefined;
  const requests: WorkerRequest[] = [];
  const terminate = vi.fn();
  const worker = {
    addEventListener(type: string, listener: unknown) {
      if (type === "message") onMessage = listener as typeof onMessage;
      if (type === "error") onError = listener as typeof onError;
    },
    postMessage(request: WorkerRequest) { requests.push(request); },
    terminate,
  } as unknown as Worker;
  return {
    client: new WorkerDesignerClient(() => worker),
    terminate,
    request(index: number): WorkerRequest {
      const request = requests[index];
      if (request === undefined) throw new Error(`Missing request ${String(index)}`);
      return request;
    },
    reply(reply: WorkerReply) {
      if (onMessage === undefined) throw new Error("Missing message listener");
      onMessage({ data: reply } as MessageEvent<WorkerReply>);
    },
    fail(message: string) {
      if (onError === undefined) throw new Error("Missing error listener");
      onError({ message } as ErrorEvent);
    },
  };
}

function table(collection: string): TableProjection {
  return {
    revision: "resident/0",
    collection: { id: collection, key: collection, entity_count: 0 },
    columns: [],
    rows: [],
  };
}

function poisonReply(id: number): WorkerReply {
  const unread = (): never => {
    throw new Error("Late reply payload was consumed after settlement.");
  };
  return {
    id,
    get status() {
      return unread();
    },
    get response() {
      return unread();
    },
    get error() {
      return unread();
    },
  };
}

describe("Designer Worker request lifecycle", () => {
  it("correlates simultaneous table replies by ID rather than arrival order", async () => {
    const worker = transport();
    const first = worker.client.queryTable("first");
    const second = worker.client.queryTable("second");
    const results = Promise.all([first, second]);
    expect(worker.request(0)).toMatchObject({ kind: "command", request: { type: "query_table", collection: "first" } });
    expect(worker.request(1)).toMatchObject({ kind: "command", request: { type: "query_table", collection: "second" } });
    expect(worker.request(0).id).not.toBe(worker.request(1).id);

    worker.reply({ id: worker.request(1).id, status: "ok", response: { type: "table", payload: table("second") } });
    worker.reply({ id: worker.request(0).id, status: "ok", response: { type: "table", payload: table("first") } });
    await expect(results).resolves.toEqual([table("first"), table("second")]);
  });

  it("ignores unknown and duplicate replies without consuming another pending request", async () => {
    const worker = transport();
    const first = worker.client.queryTable("first");
    const second = worker.client.queryTable("second");
    const results = Promise.all([first, second]);
    worker.reply({ id: -1, status: "ok", response: { type: "table", payload: table("unknown") } });
    worker.reply({ id: worker.request(0).id, status: "ok", response: { type: "table", payload: table("first") } });
    worker.reply({ id: worker.request(0).id, status: "ok", response: { type: "table", payload: table("duplicate") } });
    worker.reply({ id: worker.request(1).id, status: "ok", response: { type: "table", payload: table("second") } });
    await expect(results).resolves.toEqual([table("first"), table("second")]);
  });

  it.each([
    ["worker crashed", "worker crashed"],
    ["", "The Designer Worker stopped unexpectedly."],
  ])("rejects every in-flight request on Worker error %j", async (message, expected) => {
    const worker = transport();
    const results = Promise.allSettled([
      worker.client.queryTable("first"),
      worker.client.queryTable("second"),
    ]);
    worker.fail(message);
    const settled = await results;
    expect(settled).toHaveLength(2);
    for (const result of settled) {
      expect(result.status).toBe("rejected");
      if (result.status !== "rejected") throw new Error("Worker failure resolved a request");
      expect(result.reason).toBeInstanceOf(Error);
      expect(result.reason).toHaveProperty("message", expected);
    }
  });

  it("terminates the Worker and rejects all pending requests when the client closes", async () => {
    const worker = transport();
    const results = Promise.allSettled([
      worker.client.queryTable("first"),
      worker.client.exportProject("resident/0"),
    ]);
    worker.client.close();
    expect(worker.terminate).toHaveBeenCalledOnce();
    const settled = await results;
    expect(settled).toHaveLength(2);
    for (const result of settled) {
      expect(result.status).toBe("rejected");
      if (result.status !== "rejected") throw new Error("Closing the client resolved a request");
      expect(result.reason).toBeInstanceOf(Error);
      expect(result.reason).toHaveProperty("message", "The Designer runtime was closed.");
    }
  });

  it("retains structured runtime failure details and rejects only the addressed request", async () => {
    const worker = transport();
    const failure: FailureProjection = {
      code: "stale_revision",
      message: "The requested revision is stale.",
      current_revision: "resident/1",
      diagnostics: [{ code: "test.stale", message: "Refresh before editing.", path: "items.value" }],
    };
    const rejected = worker.client.queryFields("resident/0", []);
    const successful = worker.client.queryTable("current");
    const results = Promise.allSettled([rejected, successful]);
    worker.reply({ id: worker.request(0).id, status: "error", error: failure });
    worker.reply({ id: worker.request(1).id, status: "ok", response: { type: "table", payload: table("current") } });
    const [first, second] = await results;
    expect(first.status).toBe("rejected");
    if (first.status !== "rejected") throw new Error("Runtime failure resolved a request");
    expect(first.reason).toBeInstanceOf(DesignerRuntimeError);
    expect(first.reason).toMatchObject({ message: failure.message, failure });
    expect(second).toEqual({ status: "fulfilled", value: table("current") });
  });

  it("rejects an unexpected response type instead of exposing the wrong projection", async () => {
    const worker = transport();
    const result = expect(worker.client.queryTable("items")).rejects.toThrow(
      "Expected 'table' response, received 'fields'.",
    );
    worker.reply({ id: worker.request(0).id, status: "ok", response: { type: "fields", payload: { revision: "resident/0", fields: [] } } });
    await result;
  });

  it("ignores a duplicate reply after completion without reading its payload", async () => {
    const worker = transport();
    const result = worker.client.queryTable("completed");
    const id = worker.request(0).id;
    worker.reply({ id, status: "ok", response: { type: "table", payload: table("completed") } });
    await expect(result).resolves.toEqual(table("completed"));
    expect(() => {
      worker.reply(poisonReply(id));
    }).not.toThrow();
  });

  it("ignores a late reply after an error rejection without reading its payload", async () => {
    const worker = transport();
    const result = worker.client.queryTable("error");
    const id = worker.request(0).id;
    const failure: FailureProjection = {
      code: "test.failure",
      message: "The request failed for this lifecycle test.",
      current_revision: "resident/0",
      diagnostics: [],
    };
    worker.reply({ id, status: "error", error: failure });
    await expect(result).rejects.toMatchObject({ message: failure.message, failure });
    expect(() => {
      worker.reply(poisonReply(id));
    }).not.toThrow();
  });

  it("ignores a late reply after close rejects a request without reading its payload", async () => {
    const worker = transport();
    const result = worker.client.queryTable("closed");
    const id = worker.request(0).id;
    worker.client.close();
    await expect(result).rejects.toThrow("The Designer runtime was closed.");
    expect(() => {
      worker.reply(poisonReply(id));
    }).not.toThrow();
  });
});
