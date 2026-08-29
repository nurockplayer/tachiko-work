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
  ProjectExport,
  TableProjection,
  WorkerReply,
  WorkerRequest,
} from "./protocol.ts";

type PendingRequest = {
  resolve(response: Exclude<WorkerReply, { status: "error" }>): void;
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
      if (event.data.status === "error") {
        pending.reject(new DesignerRuntimeError(event.data.error));
      } else {
        pending.resolve(event.data);
      }
    });
    this.#worker.addEventListener("error", (event) => {
      const error = new Error(event.message || "The Designer Worker stopped unexpectedly.");
      for (const pending of this.#pending.values()) pending.reject(error);
      this.#pending.clear();
    });
  }

  async bootstrap(): Promise<BootstrapProjection> {
    return expectResponse(
      "bootstrap",
      await this.#command({
        type: "bootstrap",
        occurrence_id: crypto.randomUUID(),
      }),
    );
  }

  async openProject(bytes: ArrayBuffer): Promise<BootstrapProjection> {
    const reply = await this.#send(
      {
        id: this.#claimId(),
        kind: "open_project",
        occurrence_id: crypto.randomUUID(),
        bytes,
      },
      [bytes],
    );
    if (reply.status !== "ok") {
      throw new Error(`Expected project open response, received '${reply.status}'.`);
    }
    return expectResponse("bootstrap", reply.response);
  }

  async exportProject(expectedRevision: string): Promise<ProjectExport> {
    const reply = await this.#send({
      id: this.#claimId(),
      kind: "export_project",
      expected_revision: expectedRevision,
    });
    if (reply.status !== "project_exported") {
      throw new Error(`Expected project export, received '${reply.status}'.`);
    }
    return reply.export;
  }

  async closeProject(): Promise<void> {
    const reply = await this.#send({ id: this.#claimId(), kind: "close_project" });
    if (reply.status !== "closed") {
      throw new Error(`Expected project close, received '${reply.status}'.`);
    }
  }

  async queryTable(collection: string): Promise<TableProjection> {
    return expectResponse(
      "table",
      await this.#command({ type: "query_table", collection }),
    );
  }

  async queryFields(
    expectedRevision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection> {
    return expectResponse(
      "fields",
      await this.#command({
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
      await this.#command({
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

  async #command(request: DesignerRequest): Promise<DesignerResponse> {
    const reply = await this.#send({ id: this.#claimId(), kind: "command", request });
    if (reply.status !== "ok") {
      throw new Error(`Expected command response, received '${reply.status}'.`);
    }
    return reply.response;
  }

  #claimId(): number {
    const id = this.#nextId;
    this.#nextId += 1;
    return id;
  }

  #send(
    message: WorkerRequest,
    transfer: Transferable[] = [],
  ): Promise<Exclude<WorkerReply, { status: "error" }>> {
    return new Promise((resolve, reject) => {
      this.#pending.set(message.id, { resolve, reject });
      this.#worker.postMessage(message, transfer);
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
