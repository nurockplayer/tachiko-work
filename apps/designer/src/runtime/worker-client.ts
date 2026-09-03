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
  OpenedProjection,
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

export type DesignerWorkerFactory = () => Worker;

export class WorkerDesignerClient implements DesignerClient {
  readonly #worker: Worker;
  readonly #pending = new Map<number, PendingRequest>();
  #nextId = 1;

  constructor(createWorker: DesignerWorkerFactory) {
    this.#worker = createWorker();
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
        occurrence_id: freshOccurrenceId(),
      }),
    );
  }

  async openProject(bytes: ArrayBuffer): Promise<OpenedProjection> {
    const reply = await this.#send(
      {
        id: this.#claimId(),
        kind: "open_project",
        occurrence_id: freshOccurrenceId(),
        bytes,
      },
      [bytes],
    );
    if (reply.status !== "ok") {
      throw new Error(`Expected project open response, received '${reply.status}'.`);
    }
    return expectResponse("opened", reply.response);
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
        type: "edit_scalar",
        expected_revision: expectedRevision,
        target,
        input: { kind: "number", input },
      }),
    );
  }

  async editText(
    expectedRevision: string,
    target: FieldTarget,
    value: string,
  ): Promise<PublicationProjection> {
    return expectResponse(
      "published",
      await this.#command({
        type: "edit_scalar",
        expected_revision: expectedRevision,
        target,
        input: { kind: "text", value },
      }),
    );
  }

  async editBoolean(
    expectedRevision: string,
    target: FieldTarget,
    value: boolean,
  ): Promise<PublicationProjection> {
    return expectResponse(
      "published",
      await this.#command({
        type: "edit_scalar",
        expected_revision: expectedRevision,
        target,
        input: { kind: "boolean", value },
      }),
    );
  }

  async editDate(
    expectedRevision: string,
    target: FieldTarget,
    value: string,
  ): Promise<PublicationProjection> {
    return expectResponse(
      "published",
      await this.#command({
        type: "edit_scalar",
        expected_revision: expectedRevision,
        target,
        input: { kind: "date", value },
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

export function freshOccurrenceId(): string {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x40;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(
    16,
    20,
  )}-${hex.slice(20)}`;
}

function expectResponse(
  type: "bootstrap",
  response: DesignerResponse,
): BootstrapProjection;
function expectResponse(
  type: "opened",
  response: DesignerResponse,
): OpenedProjection;
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
