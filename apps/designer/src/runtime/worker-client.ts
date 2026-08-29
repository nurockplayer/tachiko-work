import {
  DesignerRuntimeError,
  type DesignerClient,
} from "./client.ts";
import type {
  BootstrapProjection,
  DesignerRequest,
  DesignerResponse,
  FieldBatchProjection,
  FieldTarget,
  PublicationProjection,
  TableProjection,
  WorkerReply,
  WorkerRequest,
} from "./protocol.ts";

type PendingRequest = {
  resolve(response: DesignerResponse): void;
  reject(error: Error): void;
};

export class WorkerDesignerClient implements DesignerClient {
  readonly #worker: Worker;
  readonly #pending = new Map<number, PendingRequest>();
  #nextId = 1;

  constructor() {
    this.#worker = new Worker(new URL("./designer.worker.ts", import.meta.url), {
      type: "module",
      name: "tachiko-designer-runtime",
    });
    this.#worker.addEventListener("message", (event: MessageEvent<WorkerReply>) => {
      const pending = this.#pending.get(event.data.id);
      if (pending === undefined) return;
      this.#pending.delete(event.data.id);
      if (event.data.status === "ok") {
        pending.resolve(event.data.response);
      } else {
        pending.reject(new DesignerRuntimeError(event.data.error));
      }
    });
    this.#worker.addEventListener("error", (event) => {
      const error = new Error(event.message || "The Designer Worker stopped unexpectedly.");
      for (const pending of this.#pending.values()) pending.reject(error);
      this.#pending.clear();
    });
  }

  async bootstrap(): Promise<BootstrapProjection> {
    return expectResponse("bootstrap", await this.#request({ type: "bootstrap" }));
  }

  async queryTable(collection: string): Promise<TableProjection> {
    return expectResponse(
      "table",
      await this.#request({ type: "query_table", collection }),
    );
  }

  async queryFields(
    expectedRevision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection> {
    return expectResponse(
      "fields",
      await this.#request({
        type: "query_fields",
        expected_revision: expectedRevision,
        fields,
      }),
    );
  }

  async editNumber(
    expectedRevision: string,
    target: FieldTarget,
    input: string,
  ): Promise<PublicationProjection> {
    return expectResponse(
      "published",
      await this.#request({
        type: "edit_number",
        expected_revision: expectedRevision,
        target,
        input,
      }),
    );
  }

  close(): void {
    this.#worker.terminate();
    const error = new Error("The Designer runtime was closed.");
    for (const pending of this.#pending.values()) pending.reject(error);
    this.#pending.clear();
  }

  #request(request: DesignerRequest): Promise<DesignerResponse> {
    const id = this.#nextId;
    this.#nextId += 1;
    const message: WorkerRequest = { id, request };
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#worker.postMessage(message);
    });
  }

}

function expectResponse(
  type: "bootstrap",
  response: DesignerResponse,
): BootstrapProjection;
function expectResponse(
  type: "table",
  response: DesignerResponse,
): TableProjection;
function expectResponse(
  type: "fields",
  response: DesignerResponse,
): FieldBatchProjection;
function expectResponse(
  type: "published",
  response: DesignerResponse,
): PublicationProjection;
function expectResponse(
  type: DesignerResponse["type"],
  response: DesignerResponse,
): DesignerResponse["payload"] {
  if (response.type !== type) {
    throw new Error(`Expected '${type}' response, received '${response.type}'.`);
  }
  return response.payload;
}
