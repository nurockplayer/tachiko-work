import { Worker } from "node:worker_threads";

import type {
  CommandResponse,
  RuntimeCommand,
  SemanticDocument,
  WireRequest,
  WireResult,
  WorkerReply,
  WorkerRequest,
} from "./protocol.ts";

type PendingRequest = {
  resolve(result: WireResult): void;
  reject(error: Error): void;
};

export class RuntimeClient {
  #worker: Worker;
  #nextId = 1;
  #pending = new Map<number, PendingRequest>();

  private constructor(worker: Worker) {
    this.#worker = worker;
    worker.on("message", (reply: WorkerReply) => {
      const pending = this.#pending.get(reply.id);
      if (pending === undefined) {
        return;
      }
      this.#pending.delete(reply.id);
      if (reply.ok) {
        pending.resolve(reply.result);
      } else {
        pending.reject(new Error(reply.error));
      }
    });
    worker.on("error", (error) => {
      for (const pending of this.#pending.values()) {
        pending.reject(error);
      }
      this.#pending.clear();
    });
  }

  static async spawn(wasmPath: string): Promise<RuntimeClient> {
    const worker = new Worker(new URL("./runtime-worker.ts", import.meta.url), {
      workerData: { wasmPath },
    });
    const client = new RuntimeClient(worker);
    await new Promise<void>((resolve, reject) => {
      worker.once("online", resolve);
      worker.once("error", reject);
    });
    return client;
  }

  async openSynthetic(entityCount: number): Promise<{ revision: number }> {
    const result = await this.#request({
      type: "open_synthetic",
      entity_count: entityCount,
    });
    if (result.type !== "opened") {
      throw new Error(`expected opened result, received ${result.type}`);
    }
    return { revision: result.revision };
  }

  async generateSynthetic(entityCount: number): Promise<SemanticDocument> {
    const result = await this.#request({
      type: "generate_synthetic",
      entity_count: entityCount,
    });
    if (result.type !== "generated") {
      throw new Error(`expected generated result, received ${result.type}`);
    }
    return result.document;
  }

  async open(document: SemanticDocument): Promise<{ revision: number }> {
    const result = await this.#request({ type: "open", document });
    if (result.type !== "opened") {
      throw new Error(`expected opened result, received ${result.type}`);
    }
    return { revision: result.revision };
  }

  async execute(command: RuntimeCommand): Promise<CommandResponse> {
    const result = await this.#request({ type: "execute", command });
    if (result.type !== "command") {
      throw new Error(`expected command result, received ${result.type}`);
    }
    return result.response;
  }

  async executeSnapshot(
    document: SemanticDocument,
    command: RuntimeCommand,
  ): Promise<{ document: SemanticDocument; response: CommandResponse }> {
    const result = await this.#request({
      type: "execute_snapshot",
      document,
      command,
    });
    if (result.type !== "snapshot_execution") {
      throw new Error(`expected snapshot execution, received ${result.type}`);
    }
    return { document: result.document, response: result.response };
  }

  async snapshot(): Promise<SemanticDocument> {
    const result = await this.#request({ type: "snapshot" });
    if (result.type !== "snapshot") {
      throw new Error(`expected snapshot result, received ${result.type}`);
    }
    return result.document;
  }

  async wireRequest(request: WireRequest): Promise<WireResult> {
    return this.#request(request);
  }

  async close(): Promise<void> {
    await this.#worker.terminate();
  }

  #request(request: WireRequest): Promise<WireResult> {
    const id = this.#nextId;
    this.#nextId += 1;
    const message: WorkerRequest = { id, request };
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#worker.postMessage(message);
    });
  }
}
