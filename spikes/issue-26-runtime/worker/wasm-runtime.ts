import type { WireReply, WireRequest, WireResult } from "./protocol.ts";

type WasmExports = {
  memory: WebAssembly.Memory;
  tachiko_request_reserve(length: number): number;
  tachiko_request_run(): void;
  tachiko_response_ptr(): number;
  tachiko_response_len(): number;
};

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export class WasmRuntime {
  #exports: WasmExports;

  private constructor(exports: WasmExports) {
    this.#exports = exports;
  }

  static async instantiate(bytes: Uint8Array): Promise<WasmRuntime> {
    const instantiated = await WebAssembly.instantiate(bytes, {});
    return new WasmRuntime(instantiated.instance.exports as unknown as WasmExports);
  }

  request(request: WireRequest): WireResult {
    const input = encoder.encode(JSON.stringify(request));
    const inputPointer = this.#exports.tachiko_request_reserve(input.byteLength);
    new Uint8Array(
      this.#exports.memory.buffer,
      inputPointer,
      input.byteLength,
    ).set(input);

    this.#exports.tachiko_request_run();
    const responsePointer = this.#exports.tachiko_response_ptr();
    const responseLength = this.#exports.tachiko_response_len();
    const responseBytes = new Uint8Array(
      this.#exports.memory.buffer,
      responsePointer,
      responseLength,
    ).slice();
    const reply = JSON.parse(decoder.decode(responseBytes)) as WireReply;
    if (!reply.ok) {
      throw new Error(reply.error);
    }
    return reply.result;
  }
}

